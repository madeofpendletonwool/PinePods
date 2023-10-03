import os

DEBUG_MODE = os.environ.get("DEBUG_MODE", "False") == "True"
LOGGING_LEVEL = "DEBUG" if DEBUG_MODE else "ERROR"

LOGGING_CONFIG = {
    "version": 1,
    "disable_existing_loggers": False,
    "formatters": {
        "default": {
            "format": "[%(asctime)s] [%(levelname)s] - %(name)s: %(message)s",
        }
    },
    "handlers": {
        "console": {
            "class": "logging.StreamHandler",
            "level": LOGGING_LEVEL,
            "formatter": "default",
            "stream": "ext://sys.stdout",
        },
    },
    "loggers": {
        "uvicorn": {
            "level": LOGGING_LEVEL,
            "handlers": ["console"],
        },
        "fastapi": {
            "level": LOGGING_LEVEL,
            "handlers": ["console"],
        },
        "your_application_logger_name": {  # Replace with your logger name
            "level": LOGGING_LEVEL,
            "handlers": ["console"],
        },
    },
}
