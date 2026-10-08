"""Shared finite native speech bounds; no runtime imports or authority."""

SAMPLE_RATE = 16000
MAX_AUDIO_SAMPLES = SAMPLE_RATE * 30
MAX_TEXT_BYTES = 16000
LANGUAGES = frozenset("en de fr it es pt el nl pl vi zh ar ja ko".split())
