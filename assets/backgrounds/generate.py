"""Generate the stationary tablet backdrop. Requires numpy and Pillow.

Run from any directory: python3 assets/backgrounds/generate.py
Oklab reference matrices: https://bottosson.github.io/posts/oklab/
The source field stays in float64 until its final dithered sRGB quantization.
"""
from pathlib import Path

import numpy as np
from PIL import Image, PngImagePlugin


DESTINATION = Path(__file__).parent
SEED = 20261002
CENTER = (0.76, -0.16)
SPREAD = (0.60, 0.34)
ANGLE = -0.20


def render(width, height):
    x = (np.arange(width, dtype=np.float64)[None, :] + 0.5) / width
    y = (np.arange(height, dtype=np.float64)[:, None] + 0.5) / height
    dx, dy = x - CENTER[0], y - CENTER[1]
    u = np.cos(ANGLE) * dx + np.sin(ANGLE) * dy
    v = -np.sin(ANGLE) * dx + np.cos(ANGLE) * dy
    light = np.exp(-0.5 * ((u / SPREAD[0]) ** 2 + (v / SPREAD[1]) ** 2))
    tint = np.exp(-0.5 * (((x - 0.66) / 0.76) ** 2 + ((y - 0.31) / 0.52) ** 2))

    # Independent lightness and low-chroma ocean tint; no flat color bands.
    L = 0.115 + 0.03 * (1 - y) + 0.54 * light
    a = -0.008 * tint * (1 - 0.70 * light)
    b = -0.016 * tint * (1 - 0.55 * light)
    l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
    linear = np.stack((
        4.0767416621*l - 3.3077115913*m + 0.2309699292*s,
        -1.2684380046*l + 2.6097574011*m - 0.3413193965*s,
        -0.0041960863*l - 0.7034186147*m + 1.7076147010*s,
    ), axis=-1)
    assert 0 <= linear.min() < linear.max() <= 1
    encoded = np.where(linear <= 0.0031308, 12.92 * linear,
                       1.055 * linear ** (1 / 2.4) - 0.055) * 255

    # Fixed, shared-channel sub-LSB dither, applied BEFORE rounding.
    # This preserves the mean field without visible grain or animated noise.
    noise = np.random.default_rng(SEED).uniform(-0.5, 0.5, (height, width, 1))
    pixels = np.clip(np.floor(encoded + noise + 0.5), 0, 255).astype(np.uint8)
    assert abs(np.mean(pixels - encoded)) < 0.002
    return pixels


if __name__ == '__main__':
    for name, width, height in [('landscape', 2880, 1920), ('portrait', 1920, 2880)]:
        metadata = PngImagePlugin.PngInfo()
        metadata.add(b'sRGB', b'\x00')
        output = DESTINATION / f'atmosphere-{name}.png'
        Image.fromarray(render(width, height), 'RGB').save(output, pnginfo=metadata)
        print(output)
