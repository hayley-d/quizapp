#!/usr/bin/env python3
"""Check that text on the app's brand surfaces clears WCAG AA (4.5:1).

Run from the repo root: python3 frontend/scripts/check-contrast.py

The values here mirror frontend/src/styles/globals.css. When a token changes
there, change it here too -- this file is the only place the ratios are
actually proven, and a screenshot cannot tell 4.4 from 4.6.
"""

import sys

AA_NORMAL_TEXT = 4.5


def srgb_to_linear(channel: float) -> float:
    channel /= 255
    return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4


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


WHITE = (255, 255, 255)
LIGHT_PAGE = (244, 250, 251)
DARK_PAGE = (38, 33, 56)
LIGHT_PRIMARY = (57, 160, 168)
DARK_PRIMARY = (125, 224, 226)

BRAND = (158, 84, 170)
DECK_CARD_HEADER = (211, 112, 224)
DECK_CARD_HEADER_ALPHA = 0.7
DECK_CARD_CHIP = (27, 31, 57)
DECK_CARD_CHIP_ALPHA = 0.7


def brand_button_surface(page: tuple[float, float, float]):
    del page
    return BRAND


def deck_card_chip_surface(primary: tuple[float, float, float]):
    band = composite(DECK_CARD_HEADER, DECK_CARD_HEADER_ALPHA, primary)
    return composite(DECK_CARD_CHIP, DECK_CARD_CHIP_ALPHA, band)


CHECKS = [
    ("brand button, light", brand_button_surface(LIGHT_PAGE)),
    ("brand button, dark", brand_button_surface(DARK_PAGE)),
    ("deck card chip, light", deck_card_chip_surface(LIGHT_PRIMARY)),
    ("deck card chip, dark", deck_card_chip_surface(DARK_PRIMARY)),
]


def main() -> int:
    failures = 0
    for description, surface in CHECKS:
        ratio = contrast_ratio(WHITE, surface)
        passed = ratio >= AA_NORMAL_TEXT
        failures += not passed
        rendered = tuple(round(channel) for channel in surface)
        print(
            f"{'ok  ' if passed else 'FAIL'} {description:24} "
            f"rgb{rendered!s:<18} white text {ratio:5.2f}:1"
        )
    if failures:
        print(f"\n{failures} surface(s) below {AA_NORMAL_TEXT}:1", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
