# Lattice atmosphere backdrop

`generate.py` creates the two PNGs used by `AtmosphereBackdrop`. Run it with
Python, NumPy, and Pillow; these are authoring dependencies, not app runtime
dependencies. Ordinary tablet builds use the checked-in generated PNGs.

The field uses a rotated ellipse whose origin sits beyond the top edge, a
smooth Gaussian lightness falloff, and an independent low-chroma ocean tint
in Oklab. `CENTER`, `SPREAD`, `ANGLE`, and the lightness/chroma coefficients
control the appearance without assigning colors to concentric bands.

Stationary, shared-channel uniform noise of ±0.5 encoded sRGB code values is
added before the one final 8-bit rounding step. There is no decorative grain.

The 2880×1920 and 1920×2880 assets match the configured StarLite panel in both
orientations. Full-window rendering maps their dither to physical pixels at
native panel resolution, including the app's 1.8 logical scale. Other window
sizes resample the texture and may attenuate the dithering; inspect native-size
exports when judging that detail. The composition is fixed; it does not change
with device state or foreground content.
