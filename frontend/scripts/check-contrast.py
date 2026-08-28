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


WHITE = (255, 255, 255)

LIGHT_TOKENS = {
    "primary": (0.62, 0.13, 195),
    "primary-foreground": (0.99, 0.01, 200),
    "secondary": (0.94, 0.035, 290),
    "secondary-foreground": (0.30, 0.06, 285),
    "accent-foreground": (0.99, 0.01, 330),
    "streak": (0.58, 0.19, 335),
    "success": (0.80, 0.14, 88),
    "success-foreground": (0.25, 0.05, 80),
    "destructive": (0.58, 0.20, 20),
    "destructive-foreground": (0.99, 0.01, 20),
}

DARK_TOKENS = {
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

BRAND = (158, 84, 170)

DECK_CARD_HEADER = (211, 112, 224)
DECK_CARD_HEADER_ALPHA = 0.7
DECK_CARD_CHIP = (27, 31, 57)
DECK_CARD_CHIP_ALPHA = 0.7


def token_rgb(tokens: dict[str, tuple[float, float, float]], name: str) -> tuple[float, float, float]:
    return oklch_to_srgb(*tokens[name])


def deck_card_chip_surface(primary: tuple[float, float, float]) -> tuple[float, float, float]:
    band = composite(DECK_CARD_HEADER, DECK_CARD_HEADER_ALPHA, primary)
    return composite(DECK_CARD_CHIP, DECK_CARD_CHIP_ALPHA, band)


ENFORCED = [
    ("brand button", WHITE, BRAND),
    ("deck card chip, light", WHITE, deck_card_chip_surface(token_rgb(LIGHT_TOKENS, "primary"))),
    ("deck card chip, dark", WHITE, deck_card_chip_surface(token_rgb(DARK_TOKENS, "primary"))),
    ("streak badge, light", token_rgb(LIGHT_TOKENS, "accent-foreground"), token_rgb(LIGHT_TOKENS, "streak")),
    ("streak badge, dark", token_rgb(DARK_TOKENS, "accent-foreground"), token_rgb(DARK_TOKENS, "streak")),
    ("choice unselected, light", token_rgb(LIGHT_TOKENS, "secondary-foreground"), token_rgb(LIGHT_TOKENS, "secondary")),
    ("choice unselected, dark", token_rgb(DARK_TOKENS, "secondary-foreground"), token_rgb(DARK_TOKENS, "secondary")),
    ("verdict success, light", token_rgb(LIGHT_TOKENS, "success-foreground"), token_rgb(LIGHT_TOKENS, "success")),
    ("verdict success, dark", token_rgb(DARK_TOKENS, "success-foreground"), token_rgb(DARK_TOKENS, "success")),
    ("verdict destructive, light", token_rgb(LIGHT_TOKENS, "destructive-foreground"), token_rgb(LIGHT_TOKENS, "destructive")),
    ("verdict destructive, dark", token_rgb(DARK_TOKENS, "destructive-foreground"), token_rgb(DARK_TOKENS, "destructive")),
]

RECORDED = [
    (
        "selected choice / active nav, light",
        token_rgb(LIGHT_TOKENS, "primary-foreground"),
        token_rgb(LIGHT_TOKENS, "primary"),
        "pre-existing; small text on primary (selected choice, active nav) fails AA. "
        "Deck card title is 30px = large text, floor 3:1, so it passes there. Fixing "
        "needs a palette decision affecting every deck card — deferred, see HANDOVER.",
    ),
]


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
