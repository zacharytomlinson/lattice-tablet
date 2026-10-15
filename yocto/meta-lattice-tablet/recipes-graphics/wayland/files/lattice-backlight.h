/* SPDX-License-Identifier: MIT */
#ifndef LATTICE_BACKLIGHT_H
#define LATTICE_BACKLIGHT_H

struct weston_compositor;
struct lattice_backlight;

struct lattice_backlight *
lattice_backlight_create(struct weston_compositor *compositor);
void lattice_backlight_sample(struct lattice_backlight *backlight,
			     const double sample[3]);
void lattice_backlight_destroy(struct lattice_backlight *backlight);

#endif
