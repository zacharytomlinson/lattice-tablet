/* SPDX-License-Identifier: MIT */
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <systemd/sd-bus.h>
#include <libweston/libweston.h>

#include "lattice-backlight.h"
#include "lattice-motion.h"

#define AWAKE_BRIGHTNESS 20

struct wake_touch {
	struct weston_touch_grab grab;
	struct wl_list link;
	struct lattice_backlight *owner;
};

struct lattice_backlight {
	struct weston_compositor *compositor;
	struct wl_listener idle_listener;
	struct wl_listener wake_listener;
	struct wl_event_source *release_grabs;
	struct wl_list grabs;
	struct lattice_motion motion;
	sd_bus *bus;
	bool dark;
	time_t retry_at;
};

static bool
set_brightness(struct lattice_backlight *backlight, unsigned int value)
{
	sd_bus_error error = SD_BUS_ERROR_NULL;
	int result;

	result = sd_bus_call_method(backlight->bus, "org.freedesktop.login1",
		"/org/freedesktop/login1/session/auto",
		"org.freedesktop.login1.Session", "SetBrightness",
		&error, NULL, "ssu", "backlight", "acpi_video0", value);
	if (result < 0)
		weston_log("Lattice backlight: cannot set %u: %s\n", value,
			   error.message ? error.message : strerror(-result));
	sd_bus_error_free(&error);
	return result >= 0;
}

static void
release_touch(struct wake_touch *touch)
{
	weston_touch_end_grab(touch->grab.touch);
	wl_list_remove(&touch->link);
	free(touch);
}

static void
ignore_touch(struct weston_touch_grab *grab, const struct timespec *time,
	     int id, struct weston_coord_global pos)
{
	(void)grab; (void)time; (void)id; (void)pos;
}

static void
wake_touch_up(struct weston_touch_grab *grab, const struct timespec *time, int id)
{
	struct wake_touch *touch = wl_container_of(grab, touch, grab);
	(void)time; (void)id;
	if (grab->touch->num_tp == 0 && !touch->owner->dark)
		release_touch(touch);
}

static void
ignore_frame(struct weston_touch_grab *grab)
{
	(void)grab;
}

static void
cancel_touch(struct weston_touch_grab *grab)
{
	struct wake_touch *touch = wl_container_of(grab, touch, grab);
	release_touch(touch);
}

static const struct weston_touch_grab_interface wake_touch_interface = {
	.down = ignore_touch,
	.up = wake_touch_up,
	.motion = ignore_touch,
	.frame = ignore_frame,
	.cancel = cancel_touch,
};

/* Called after the wake input has been dispatched. Keep swallowing a wake
 * gesture until every finger lifts; movement-only wakes release immediately.
 */
static void
release_unused_grabs(void *data)
{
	struct lattice_backlight *backlight = data;
	struct wake_touch *touch, *next;
	backlight->release_grabs = NULL;
	wl_list_for_each_safe(touch, next, &backlight->grabs, link) {
		if (touch->grab.touch->num_tp == 0)
			release_touch(touch);
	}
}

static void
handle_wake(struct wl_listener *listener, void *data)
{
	struct lattice_backlight *backlight =
		wl_container_of(listener, backlight, wake_listener);
	(void)data;
	if (backlight->dark && set_brightness(backlight, AWAKE_BRIGHTNESS)) {
		backlight->dark = false;
		weston_log("Lattice backlight: awake at %d\n", AWAKE_BRIGHTNESS);
	}
	if (backlight->dark)
		return;
	if (!backlight->release_grabs)
		backlight->release_grabs = wl_event_loop_add_idle(
			wl_display_get_event_loop(backlight->compositor->wl_display),
			release_unused_grabs, backlight);
}

static void
handle_idle(struct wl_listener *listener, void *data)
{
	struct lattice_backlight *backlight =
		wl_container_of(listener, backlight, idle_listener);
	struct weston_seat *seat;
	(void)data;

	/* The compositor's idle inhibition already prevents blanking mid-gesture. */
	wl_list_for_each(seat, &backlight->compositor->seat_list, link) {
		struct weston_touch *touch = weston_seat_get_touch(seat);
		struct wake_touch *grab;
		if (!touch)
			continue;
		if (touch->grab != &touch->default_grab)
			goto stay_awake;
		grab = calloc(1, sizeof *grab);
		if (!grab)
			goto stay_awake;
		grab->grab.interface = &wake_touch_interface;
		grab->owner = backlight;
		wl_list_insert(&backlight->grabs, &grab->link);
		weston_touch_start_grab(touch, &grab->grab);
	}
	if (!set_brightness(backlight, 0))
		goto stay_awake;
	backlight->dark = true;
	weston_log("Lattice backlight: idle at 0\n");
	return;

stay_awake:
	weston_compositor_wake(backlight->compositor);
}

struct lattice_backlight *
lattice_backlight_create(struct weston_compositor *compositor)
{
	struct lattice_backlight *backlight = calloc(1, sizeof *backlight);
	if (!backlight)
		return NULL;
	backlight->compositor = compositor;
	wl_list_init(&backlight->grabs);
	if (sd_bus_open_system(&backlight->bus) < 0)
		goto fail;
	/* Never let a local service failure stall input for D-Bus's default timeout. */
	sd_bus_set_method_call_timeout(backlight->bus, 250000);
	if (!set_brightness(backlight, AWAKE_BRIGHTNESS))
		goto fail;
	backlight->idle_listener.notify = handle_idle;
	backlight->wake_listener.notify = handle_wake;
	wl_signal_add(&compositor->idle_signal, &backlight->idle_listener);
	wl_signal_add(&compositor->wake_signal, &backlight->wake_listener);
	weston_log("Lattice backlight: motion/touch idle timeout %d s, awake %d\n",
		   compositor->idle_time, AWAKE_BRIGHTNESS);
	return backlight;

fail:
	weston_log("Lattice backlight: idle control unavailable; keeping display awake\n");
	compositor->idle_time = 0;
	sd_bus_unref(backlight->bus);
	free(backlight);
	return NULL;
}

void
lattice_backlight_sample(struct lattice_backlight *backlight, const double sample[3])
{
	struct timespec now;
	if (!backlight)
		return;
	if (lattice_motion_update(&backlight->motion, sample))
		weston_compositor_wake(backlight->compositor);
	/* Retry restoration after a transient logind failure, without a new timer. */
	if (backlight->dark && backlight->compositor->state == WESTON_COMPOSITOR_ACTIVE) {
		clock_gettime(CLOCK_MONOTONIC, &now);
		if (now.tv_sec >= backlight->retry_at) {
			backlight->retry_at = now.tv_sec + 1;
			handle_wake(&backlight->wake_listener, NULL);
		}
	}
}

void
lattice_backlight_destroy(struct lattice_backlight *backlight)
{
	struct wake_touch *touch, *next;
	if (!backlight)
		return;
	if (backlight->dark)
		set_brightness(backlight, AWAKE_BRIGHTNESS);
	if (backlight->release_grabs)
		wl_event_source_remove(backlight->release_grabs);
	wl_list_for_each_safe(touch, next, &backlight->grabs, link)
		release_touch(touch);
	wl_list_remove(&backlight->idle_listener.link);
	wl_list_remove(&backlight->wake_listener.link);
	sd_bus_unref(backlight->bus);
	free(backlight);
}
