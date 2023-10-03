import logging
import os

DEBUG = os.environ.get("DEBUG_MODE", "False") == "True"

print(f"Logging options set to debug: {DEBUG}")

LOGGING_CONFIG = {
    "version": 1,
    "disable_existing_loggers": False,
    "handlers": {
        "console": {
            "class": "logging.StreamHandler",
            "level": "DEBUG" if DEBUG else "ERROR",
        }
    },
    "root": {"level": "DEBUG" if DEBUG else "ERROR", "handlers": ["console"]},
}
