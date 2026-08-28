#!/usr/bin/env python3

import math
import sys

AA_NORMAL_TEXT = 4.5


def srgb_to_linear(channel: float) -> float:
    channel /= 255
    return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4


def linear_to_srgb(channel: float) -> float:
    channel = max(0.0, min(1.0, channel))
    if channel <= 0.0031308:
        value = channel * 12.92
    else:
        value = 1.055 * channel ** (1 / 2.4) - 0.055
    return value * 255


def luminance(colour: tuple[float, float, float]) -> float:
    red, green, blue = (srgb_to_linear(channel) for channel in colour)
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast_ratio(
    foreground: tuple[float, float, float],
    background: tuple[float, float, float],
) -> float:
    lighter, darker = sorted(
        (luminance(foreground), luminance(background)), reverse=True
    )
    return (lighter + 0.05) / (darker + 0.05)


def composite(
    source: tuple[float, float, float],
    alpha: float,
    backdrop: tuple[float, float, float],
) -> tuple[float, float, float]:
    return tuple(
        alpha * channel + (1 - alpha) * behind
        for channel, behind in zip(source, backdrop)
    )


def oklch_to_srgb(lightness: float, chroma: float, hue_degrees: float) -> tuple[float, float, float]:
    hue_radians = math.radians(hue_degrees)
    oklab_a = chroma * math.cos(hue_radians)
    oklab_b = chroma * math.sin(hue_radians)

    long_response = lightness + 0.3963377774 * oklab_a + 0.2158037573 * oklab_b
    medium_response = lightness - 0.1055613458 * oklab_a - 0.0638541728 * oklab_b
    short_response = lightness - 0.0894841775 * oklab_a - 1.2914855480 * oklab_b

    long_cubed = long_response ** 3
    medium_cubed = medium_response ** 3
    short_cubed = short_response ** 3

    red_linear = 4.0767416621 * long_cubed - 3.3077115913 * medium_cubed + 0.2309699292 * short_cubed
    green_linear = -1.2684380046 * long_cubed + 2.6097574011 * medium_cubed - 0.3413193965 * short_cubed
    blue_linear = -0.0041960863 * long_cubed - 0.7034186147 * medium_cubed + 1.7076147010 * short_cubed

    return tuple(linear_to_srgb(channel) for channel in (red_linear, green_linear, blue_linear))


def hex_to_rgb(hex_colour: str) -> tuple[float, float, float]:
    hex_colour = hex_colour.lstrip("#")
    return tuple(int(hex_colour[index:index + 2], 16) for index in (0, 2, 4))


WHITE = (255, 255, 255)

LIGHT_TOKENS_OKLCH = {
    "foreground": (0.29, 0.020, 70),
    "primary": (0.47, 0.075, 68),
    "secondary-foreground": (0.30, 0.030, 70),
    "accent": (0.53, 0.085, 40),
    "muted-foreground": (0.45, 0.025, 70),
    "success": (0.72, 0.115, 88),
    "success-foreground": (0.26, 0.045, 80),
    "destructive": (0.52, 0.150, 28),
    "border": (0.80, 0.022, 70),
    "streak": (0.53, 0.085, 40),
    "brand": (0.47, 0.075, 68),
    "deck-card-chip": (0.47, 0.075, 68),
    "deck-card-foreground": (0.29, 0.020, 70),
}

LIGHT_TOKENS_HEX = {
    "background": "#f5ebe0",
    "card": "#e3d5ca",
    "primary-foreground": "#f5ebe0",
    "secondary": "#d5bdaf",
    "accent-foreground": "#f5ebe0",
    "muted": "#d5bdaf",
    "destructive-foreground": "#f5ebe0",
    "brand-foreground": "#f5ebe0",
    "deck-card": "#e3d5ca",
    "deck-card-header": "#d5bdaf",
    "deck-card-chip-foreground": "#f5ebe0",
}

LIGHT_RGB = {name: oklch_to_srgb(*value) for name, value in LIGHT_TOKENS_OKLCH.items()}
LIGHT_RGB.update({name: hex_to_rgb(value) for name, value in LIGHT_TOKENS_HEX.items()})

DARK_TOKENS_OKLCH = {
    "primary": (0.78, 0.14, 190),
    "primary-foreground": (0.18, 0.04, 270),
    "secondary": (0.34, 0.06, 290),
    "secondary-foreground": (0.94, 0.02, 290),
    "accent-foreground": (0.16, 0.04, 330),
    "streak": (0.74, 0.20, 335),
    "success": (0.84, 0.15, 88),
    "success-foreground": (0.20, 0.05, 80),
    "destructive": (0.65, 0.19, 22),
    "destructive-foreground": (0.16, 0.04, 20),
}

DARK_RGB = {name: oklch_to_srgb(*value) for name, value in DARK_TOKENS_OKLCH.items()}

DARK_BRAND = (158, 84, 170)
DARK_BRAND_FOREGROUND = WHITE

DARK_DECK_CARD_HEADER = (211, 112, 224)
DARK_DECK_CARD_HEADER_ALPHA = 0.7
DARK_DECK_CARD_CHIP = (27, 31, 57)
DARK_DECK_CARD_CHIP_ALPHA = 0.7
DARK_DECK_CARD_CHIP_FOREGROUND = WHITE


def dark_deck_card_chip_surface(primary: tuple[float, float, float]) -> tuple[float, float, float]:
    band = composite(DARK_DECK_CARD_HEADER, DARK_DECK_CARD_HEADER_ALPHA, primary)
    return composite(DARK_DECK_CARD_CHIP, DARK_DECK_CARD_CHIP_ALPHA, band)


ENFORCED = [
    ("brand button, light", LIGHT_RGB["brand-foreground"], LIGHT_RGB["brand"]),
    ("brand button, dark", DARK_BRAND_FOREGROUND, DARK_BRAND),
    ("selected choice / active nav, light", LIGHT_RGB["primary-foreground"], LIGHT_RGB["primary"]),
    ("selected choice / active nav, dark", DARK_RGB["primary-foreground"], DARK_RGB["primary"]),
    ("deck card title, light", LIGHT_RGB["deck-card-foreground"], LIGHT_RGB["deck-card"]),
    ("deck card title, dark", DARK_RGB["primary-foreground"], DARK_RGB["primary"]),
    ("deck card chip, light", LIGHT_RGB["deck-card-chip-foreground"], LIGHT_RGB["deck-card-chip"]),
    (
        "deck card chip, dark",
        DARK_DECK_CARD_CHIP_FOREGROUND,
        dark_deck_card_chip_surface(DARK_RGB["primary"]),
    ),
    ("streak badge, light", LIGHT_RGB["accent-foreground"], LIGHT_RGB["streak"]),
    ("streak badge, dark", DARK_RGB["accent-foreground"], DARK_RGB["streak"]),
    ("choice unselected, light", LIGHT_RGB["secondary-foreground"], LIGHT_RGB["secondary"]),
    ("choice unselected, dark", DARK_RGB["secondary-foreground"], DARK_RGB["secondary"]),
    ("verdict success, light", LIGHT_RGB["success-foreground"], LIGHT_RGB["success"]),
    ("verdict success, dark", DARK_RGB["success-foreground"], DARK_RGB["success"]),
    ("verdict destructive, light", LIGHT_RGB["destructive-foreground"], LIGHT_RGB["destructive"]),
    ("verdict destructive, dark", DARK_RGB["destructive-foreground"], DARK_RGB["destructive"]),
]

RECORDED: list[tuple[str, tuple[float, float, float], tuple[float, float, float], str]] = []


def main() -> int:
    failures = 0
    for description, foreground, background in ENFORCED:
        ratio = contrast_ratio(foreground, background)
        passed = ratio >= AA_NORMAL_TEXT
        failures += not passed
        rendered = tuple(round(channel) for channel in background)
        print(
            f"{'ok  ' if passed else 'FAIL'} {description:32} "
            f"rgb{rendered!s:<18} {ratio:5.2f}:1"
        )
    if RECORDED:
        print("\nRECORDED (not enforced):")
        for description, foreground, background, reason in RECORDED:
            ratio = contrast_ratio(foreground, background)
            rendered = tuple(round(channel) for channel in background)
            print(
                f"KNOWN {description:31} "
                f"rgb{rendered!s:<18} {ratio:5.2f}:1  {reason}"
            )
    if failures:
        print(f"\n{failures} enforced surface(s) below {AA_NORMAL_TEXT}:1", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
