from typing import Dict, Tuple
from threading import Lock

class ImportProgressManager:
    def __init__(self):
        self.progress: Dict[int, Tuple[int, int, str]] = {}  # (current, total, current_podcast)
        self.lock = Lock()

    def start_import(self, user_id: int, total_podcasts: int):
        with self.lock:
            self.progress[user_id] = (0, total_podcasts, "")

    def update_progress(self, user_id: int, current: int, current_podcast: str):
        with self.lock:
            _, total, _ = self.progress.get(user_id, (0, 0, ""))
            self.progress[user_id] = (current, total, current_podcast)

    def get_progress(self, user_id: int) -> Tuple[int, int, str]:
        with self.lock:
            return self.progress.get(user_id, (0, 0, ""))

    def clear_progress(self, user_id: int):
        with self.lock:
            self.progress.pop(user_id, None)

import_progress_manager = ImportProgressManager()