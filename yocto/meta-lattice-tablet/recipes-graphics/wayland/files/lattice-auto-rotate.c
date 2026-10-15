/*
 * Copyright (c) 2026 Lattice Tablet contributors
 * SPDX-License-Identifier: MIT
 */

#include <dirent.h>
#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <libweston/libweston.h>
#include "lattice-backlight.h"

#define SENSOR_ROOT "/sys/bus/iio/devices"
#define POLL_MS 100
#define STABLE_SAMPLES 3
#define RETRY_SAMPLES 14
#define PLANE_GRAVITY_RATIO 0.70
#define AXIS_DOMINANCE_RATIO 1.25
#define TRANSITION_MS 280.0
#define FIT_SCALE_INFLUENCE 0.30
#define PI 3.14159265358979323846

struct lattice_auto_rotate;

struct animated_view {
	struct wl_list link;
	struct weston_view *view;
	struct weston_transform transform;
	struct wl_listener destroy_listener;
};

struct rotation_transition {
	struct weston_animation animation;
	struct lattice_auto_rotate *owner;
	struct weston_output *output;
	struct wl_list views;
	struct timespec started_at;
	double starting_angle;
	struct wl_event_source *watchdog;
	bool started;
};

enum lattice_orientation {
	LATTICE_ORIENTATION_UNDEFINED = -1,
	LATTICE_ORIENTATION_NORMAL = WL_OUTPUT_TRANSFORM_NORMAL,
	LATTICE_ORIENTATION_LEFT_UP = WL_OUTPUT_TRANSFORM_90,
	LATTICE_ORIENTATION_BOTTOM_UP = WL_OUTPUT_TRANSFORM_180,
	LATTICE_ORIENTATION_RIGHT_UP = WL_OUTPUT_TRANSFORM_270,
};

struct lattice_auto_rotate {
	struct weston_compositor *compositor;
	struct wl_event_source *timer;
	struct wl_listener destroy_listener;
	char sensor_path[512];
	double mount_matrix[9];
	enum lattice_orientation candidate;
	unsigned int candidate_samples;
	unsigned int discovery_samples;
	struct rotation_transition *transition;
	struct lattice_backlight *backlight;
};

static const char *
orientation_name(enum lattice_orientation orientation)
{
	switch (orientation) {
	case LATTICE_ORIENTATION_NORMAL:
		return "normal";
	case LATTICE_ORIENTATION_LEFT_UP:
		return "left-up";
	case LATTICE_ORIENTATION_BOTTOM_UP:
		return "bottom-up";
	case LATTICE_ORIENTATION_RIGHT_UP:
		return "right-up";
	case LATTICE_ORIENTATION_UNDEFINED:
		break;
	}
	return "undefined";
}

static bool
read_text_file(const char *path, char *buffer, size_t size)
{
	FILE *file = fopen(path, "r");

	if (!file)
		return false;
	if (!fgets(buffer, size, file)) {
		fclose(file);
		return false;
	}

	fclose(file);
	buffer[strcspn(buffer, "\r\n")] = '\0';
	return true;
}

static bool
read_sensor_value(const struct lattice_auto_rotate *rotation,
		  const char *attribute, double *value)
{
	char path[640];
	char text[128];
	char *end;
	double parsed;

	if (snprintf(path, sizeof path, "%s/%s", rotation->sensor_path,
		     attribute) >= (int)sizeof path)
		return false;
	if (!read_text_file(path, text, sizeof text))
		return false;

	errno = 0;
	parsed = strtod(text, &end);
	if (errno || end == text)
		return false;

	*value = parsed;
	return true;
}

static bool
parse_mount_matrix(const char *text, double matrix[9])
{
	char normalized[512];
	char *cursor;
	char *end;
	unsigned int i;

	if (strlen(text) >= sizeof normalized)
		return false;
	strcpy(normalized, text);
	for (cursor = normalized; *cursor; cursor++) {
		if (*cursor == ',' || *cursor == ';')
			*cursor = ' ';
	}

	cursor = normalized;
	for (i = 0; i < 9; i++) {
		errno = 0;
		matrix[i] = strtod(cursor, &end);
		if (errno || end == cursor)
			return false;
		cursor = end;
	}

	return true;
}

static void
load_mount_matrix(struct lattice_auto_rotate *rotation)
{
	char path[640];
	char text[512];
	unsigned int i;

	if (snprintf(path, sizeof path, "%s/in_accel_mount_matrix",
		     rotation->sensor_path) < (int)sizeof path &&
	    read_text_file(path, text, sizeof text) &&
	    parse_mount_matrix(text, rotation->mount_matrix)) {
		weston_log("Lattice rotation: sensor mount matrix %s\n", text);
		return;
	}

	for (i = 0; i < 9; i++)
		rotation->mount_matrix[i] = (i % 4 == 0) ? 1.0 : 0.0;
	weston_log("Lattice rotation: no valid mount matrix; using identity\n");
}

static bool
discover_sensor(struct lattice_auto_rotate *rotation)
{
	struct dirent *entry;
	DIR *directory;
	char path[640];
	char name[128];

	directory = opendir(SENSOR_ROOT);
	if (!directory)
		return false;

	while ((entry = readdir(directory))) {
		if (strncmp(entry->d_name, "iio:device", 10) != 0)
			continue;
		if (snprintf(path, sizeof path, "%s/%s/name", SENSOR_ROOT,
			     entry->d_name) >= (int)sizeof path)
			continue;
		if (!read_text_file(path, name, sizeof name))
			continue;
		if (!strstr(name, "KIOX000A") &&
		    strncmp(name, "kxcjk1013", 10) != 0)
			continue;

		snprintf(rotation->sensor_path, sizeof rotation->sensor_path,
			 "%s/%s", SENSOR_ROOT, entry->d_name);
		closedir(directory);
		load_mount_matrix(rotation);
		weston_log("Lattice rotation: using %s (%s)\n",
			   rotation->sensor_path, name);
		return true;
	}

	closedir(directory);
	return false;
}

static enum lattice_orientation
classify_orientation(double x, double y, double z)
{
	double absolute_x = fabs(x);
	double absolute_y = fabs(y);
	double plane = hypot(x, y);
	double gravity = hypot(plane, z);

	/* Retain orientation while flat and near diagonals to prevent chatter. */
	if (gravity == 0.0 || plane < gravity * PLANE_GRAVITY_RATIO)
		return LATTICE_ORIENTATION_UNDEFINED;
	if (absolute_x > absolute_y * AXIS_DOMINANCE_RATIO)
		return x > 0.0 ? LATTICE_ORIENTATION_LEFT_UP
			       : LATTICE_ORIENTATION_RIGHT_UP;
	if (absolute_y > absolute_x * AXIS_DOMINANCE_RATIO)
		return y > 0.0 ? LATTICE_ORIENTATION_NORMAL
			       : LATTICE_ORIENTATION_BOTTOM_UP;

	return LATTICE_ORIENTATION_UNDEFINED;
}

static void
remove_animated_view(struct animated_view *animated)
{
	wl_list_remove(&animated->destroy_listener.link);
	weston_view_remove_transform(animated->view, &animated->transform);
	wl_list_remove(&animated->link);
	free(animated);
}

static void
handle_animated_view_destroy(struct wl_listener *listener, void *data)
{
	struct animated_view *animated =
		wl_container_of(listener, animated, destroy_listener);

	(void)data;
	remove_animated_view(animated);
}

static void
finish_transition(struct rotation_transition *transition)
{
	struct animated_view *animated;
	struct animated_view *next;

	wl_list_remove(&transition->animation.link);
	if (transition->watchdog)
		wl_event_source_remove(transition->watchdog);
	wl_list_for_each_safe(animated, next, &transition->views, link)
		remove_animated_view(animated);
	transition->owner->transition = NULL;
	weston_output_schedule_repaint(transition->output);
	free(transition);
}

static void
update_transition(struct rotation_transition *transition, double progress)
{
	struct animated_view *animated;
	double eased = progress * progress * (3.0 - 2.0 * progress);
	double angle = transition->starting_angle * (1.0 - eased);
	double cosine = cos(angle);
	double sine = sin(angle);

	wl_list_for_each(animated, &transition->views, link) {
		double width = animated->view->surface->width;
		double height = animated->view->surface->height;
		double bounds_width;
		double bounds_height;
		double scale;

		if (width <= 0.0 || height <= 0.0)
			continue;
		bounds_width = fabs(cosine) * width + fabs(sine) * height;
		bounds_height = fabs(sine) * width + fabs(cosine) * height;
		scale = fmin(1.0,
			     fmin(transition->output->width / bounds_width,
				  transition->output->height / bounds_height));
		scale = 1.0 - (1.0 - scale) * FIT_SCALE_INFLUENCE;

		weston_matrix_init(&animated->transform.matrix);
		weston_matrix_translate(&animated->transform.matrix,
					-width / 2.0, -height / 2.0, 0.0);
		weston_matrix_scale(&animated->transform.matrix,
				    scale, scale, 1.0);
		weston_matrix_rotate_xy(&animated->transform.matrix,
					cosine, sine);
		weston_matrix_translate(&animated->transform.matrix,
					width / 2.0, height / 2.0, 0.0);
		weston_view_add_transform(
			animated->view,
			&animated->view->geometry.transformation_list,
			&animated->transform);
	}
}

static double
elapsed_milliseconds(const struct timespec *start, const struct timespec *now)
{
	return (now->tv_sec - start->tv_sec) * 1000.0 +
	       (now->tv_nsec - start->tv_nsec) / 1000000.0;
}

static void
transition_frame(struct weston_animation *animation,
		 struct weston_output *output, const struct timespec *time)
{
	struct rotation_transition *transition =
		wl_container_of(animation, transition, animation);
	double progress;

	(void)output;
	if (!transition->started) {
		transition->started_at = *time;
		transition->started = true;
	}
	progress = elapsed_milliseconds(&transition->started_at, time) /
		   TRANSITION_MS;
	if (progress >= 1.0) {
		finish_transition(transition);
		return;
	}

	update_transition(transition, fmax(0.0, progress));
	weston_compositor_schedule_repaint(transition->owner->compositor);
}

static int
transition_watchdog(void *data)
{
	struct rotation_transition *transition = data;

	transition->watchdog = NULL;
	weston_log("Lattice rotation: transition watchdog completed cleanup\n");
	finish_transition(transition);
	return 0;
}

static void
start_transition(struct lattice_auto_rotate *rotation,
		 struct weston_output *output, uint32_t old_transform,
		 enum lattice_orientation orientation)
{
	struct rotation_transition *transition;
	struct weston_view *view;
	struct wl_event_loop *loop;
	int quarter_turns;

	if (rotation->transition)
		finish_transition(rotation->transition);

	transition = calloc(1, sizeof *transition);
	if (!transition) {
		weston_output_schedule_repaint(output);
		return;
	}
	transition->owner = rotation;
	transition->output = output;
	wl_list_init(&transition->views);

	quarter_turns = ((int)orientation - (int)old_transform + 4) % 4;
	if (quarter_turns == 3)
		quarter_turns = -1;
	transition->starting_angle = quarter_turns * PI / 2.0;
	transition->animation.frame = transition_frame;
	transition->animation.frame_counter = 0;

	wl_list_for_each(view, &rotation->compositor->view_list, link) {
		struct animated_view *animated;

		if (!view->is_mapped || view->output != output)
			continue;
		animated = calloc(1, sizeof *animated);
		if (!animated)
			continue;
		animated->view = view;
		wl_list_init(&animated->transform.link);
		animated->destroy_listener.notify = handle_animated_view_destroy;
		wl_signal_add(&view->destroy_signal, &animated->destroy_listener);
		wl_list_insert(&transition->views, &animated->link);
	}

	if (wl_list_empty(&transition->views)) {
		free(transition);
		weston_output_schedule_repaint(output);
		return;
	}

	rotation->transition = transition;
	update_transition(transition, 0.0);
	wl_list_insert(&output->animation_list, &transition->animation.link);
	loop = wl_display_get_event_loop(rotation->compositor->wl_display);
	transition->watchdog =
		wl_event_loop_add_timer(loop, transition_watchdog, transition);
	if (transition->watchdog)
		wl_event_source_timer_update(transition->watchdog, 500);
	weston_compositor_schedule_repaint(rotation->compositor);
}

static void
apply_orientation(struct lattice_auto_rotate *rotation,
		  enum lattice_orientation orientation)
{
	struct weston_output *output;

	wl_list_for_each(output, &rotation->compositor->output_list, link) {
		if (output->transform == (uint32_t)orientation)
			continue;
		uint32_t old_transform = output->transform;

		weston_log("Lattice rotation: %s -> %s\n", output->name,
			   orientation_name(orientation));
		weston_output_set_transform(output, (uint32_t)orientation);
		wl_signal_emit(&rotation->compositor->output_heads_changed_signal,
			       output);
		wl_signal_emit(&rotation->compositor->output_resized_signal, output);
		start_transition(rotation, output, old_transform, orientation);
	}
}

static int
poll_sensor(void *data)
{
	struct lattice_auto_rotate *rotation = data;
	double raw_x, raw_y, raw_z;
	double x, y, z;
	enum lattice_orientation orientation;

	if (!rotation->sensor_path[0]) {
		lattice_backlight_sample(rotation->backlight, NULL);
		if (!discover_sensor(rotation) &&
		    ++rotation->discovery_samples % RETRY_SAMPLES == 0)
			weston_log("Lattice rotation: waiting for a Kionix IIO sensor\n");
		goto rearm;
	}

	if (!read_sensor_value(rotation, "in_accel_x_raw", &raw_x) ||
	    !read_sensor_value(rotation, "in_accel_y_raw", &raw_y) ||
	    !read_sensor_value(rotation, "in_accel_z_raw", &raw_z)) {
		lattice_backlight_sample(rotation->backlight, NULL);
		rotation->sensor_path[0] = '\0';
		rotation->candidate = LATTICE_ORIENTATION_UNDEFINED;
		rotation->candidate_samples = 0;
		goto rearm;
	}

	lattice_backlight_sample(rotation->backlight,
			       (double[]){raw_x, raw_y, raw_z});

	x = rotation->mount_matrix[0] * raw_x +
	    rotation->mount_matrix[1] * raw_y +
	    rotation->mount_matrix[2] * raw_z;
	y = rotation->mount_matrix[3] * raw_x +
	    rotation->mount_matrix[4] * raw_y +
	    rotation->mount_matrix[5] * raw_z;
	z = rotation->mount_matrix[6] * raw_x +
	    rotation->mount_matrix[7] * raw_y +
	    rotation->mount_matrix[8] * raw_z;

	orientation = classify_orientation(x, y, z);
	if (orientation == LATTICE_ORIENTATION_UNDEFINED) {
		rotation->candidate = orientation;
		rotation->candidate_samples = 0;
		goto rearm;
	}

	if (orientation != rotation->candidate) {
		rotation->candidate = orientation;
		rotation->candidate_samples = 1;
	} else if (rotation->candidate_samples < STABLE_SAMPLES) {
		rotation->candidate_samples++;
	}

	if (rotation->candidate_samples == STABLE_SAMPLES) {
		apply_orientation(rotation, orientation);
		rotation->candidate_samples++;
	}

rearm:
	wl_event_source_timer_update(rotation->timer, POLL_MS);
	return 0;
}

static void
destroy_rotation(struct wl_listener *listener, void *data)
{
	struct lattice_auto_rotate *rotation =
		wl_container_of(listener, rotation, destroy_listener);

	(void)data;
	lattice_backlight_destroy(rotation->backlight);
	if (rotation->transition)
		finish_transition(rotation->transition);
	if (rotation->timer)
		wl_event_source_remove(rotation->timer);
	wl_list_remove(&rotation->destroy_listener.link);
	free(rotation);
}

WL_EXPORT int
wet_module_init(struct weston_compositor *compositor, int *argc, char *argv[])
{
	struct lattice_auto_rotate *rotation;
	struct wl_event_loop *loop;
	const char *enabled = getenv("LATTICE_AUTO_ROTATE");

	(void)argc;
	(void)argv;

	if (enabled && (!strcmp(enabled, "0") || !strcmp(enabled, "false"))) {
		weston_log("Lattice rotation: disabled by LATTICE_AUTO_ROTATE\n");
		return 0;
	}

	rotation = calloc(1, sizeof *rotation);
	if (!rotation)
		return -1;

	rotation->compositor = compositor;
	rotation->candidate = LATTICE_ORIENTATION_UNDEFINED;
	rotation->destroy_listener.notify = destroy_rotation;
	wl_signal_add(&compositor->destroy_signal, &rotation->destroy_listener);

	loop = wl_display_get_event_loop(compositor->wl_display);
	rotation->timer = wl_event_loop_add_timer(loop, poll_sensor, rotation);
	if (!rotation->timer) {
		wl_list_remove(&rotation->destroy_listener.link);
		free(rotation);
		return -1;
	}

	weston_log("Lattice rotation: accelerometer monitor enabled\n");
	rotation->backlight = lattice_backlight_create(compositor);
	wl_event_source_timer_update(rotation->timer, 1);
	return 0;
}
