/* SPDX-License-Identifier: MIT */
#ifndef LATTICE_MOTION_H
#define LATTICE_MOTION_H

#include <math.h>
#include <stdbool.h>

struct lattice_motion {
	double reference[3];
	bool initialized;
};

/* Compare against the last activity anchor, not just the preceding sample:
 * this catches a slow pickup without treating stationary sensor noise as use.
 * The threshold is 4% of gravity, independent of the sensor's raw-unit scale.
 * An unavailable/invalid sensor keeps the display awake.
 */
static inline bool
lattice_motion_update(struct lattice_motion *motion, const double sample[3])
{
	double gravity, delta;

	if (!sample || !isfinite(sample[0]) || !isfinite(sample[1]) ||
	    !isfinite(sample[2])) {
		motion->initialized = false;
		return true;
	}
	gravity = hypot(hypot(sample[0], sample[1]), sample[2]);
	if (gravity < 1.0) {
		motion->initialized = false;
		return true;
	}
	if (motion->initialized) {
		delta = hypot(hypot(sample[0] - motion->reference[0],
				    sample[1] - motion->reference[1]),
			      sample[2] - motion->reference[2]);
		if (delta < 0.04 * gravity)
			return false;
	}
	for (unsigned int i = 0; i < 3; i++)
		motion->reference[i] = sample[i];
	motion->initialized = true;
	return true;
}
#endif
