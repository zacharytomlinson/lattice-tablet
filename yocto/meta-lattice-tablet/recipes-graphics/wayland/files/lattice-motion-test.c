/* SPDX-License-Identifier: MIT */
#include <assert.h>
#include <stdio.h>
#include "lattice-motion.h"

int main(void)
{
	struct lattice_motion motion = {0};
	double baseline[] = {19, 29, -530};
	double noise[] = {23, 31, -537};
	double pickup[] = {45, 29, -530};
	double slow[] = {19, 29, -530};
	double invalid[] = {NAN, 0, 0};

	assert(lattice_motion_update(&motion, baseline));
	/* More than five minutes of observed stationary noise must remain idle. */
	for (unsigned int i = 0; i < 4000; i++)
		assert(!lattice_motion_update(&motion, i % 2 ? baseline : noise));
	assert(lattice_motion_update(&motion, pickup));
	assert(!lattice_motion_update(&motion, pickup));

	/* Small successive movements accumulate relative to the last activity. */
	motion = (struct lattice_motion){0};
	assert(lattice_motion_update(&motion, slow));
	unsigned int detected = 0;
	for (unsigned int i = 0; i < 30; i++) {
		slow[0] += 1;
		detected += lattice_motion_update(&motion, slow);
	}
	assert(detected > 0);
	assert(lattice_motion_update(&motion, NULL));
	assert(lattice_motion_update(&motion, baseline));
	assert(lattice_motion_update(&motion, invalid));
	assert(lattice_motion_update(&motion, (double[]){0, 0, 0}));
	puts("motion: stationary noise, pickup, slow movement, and sensor loss passed");
}
