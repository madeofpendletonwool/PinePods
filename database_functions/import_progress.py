import json
from valkey_client import valkey_client

class ImportProgressManager:
    def start_import(self, user_id: int, total_podcasts: int):
        valkey_client.set(f"import_progress:{user_id}", json.dumps({
            "current": 0,
            "total": total_podcasts,
            "current_podcast": ""
        }))

    def update_progress(self, user_id: int, current: int, current_podcast: str):
        progress_json = valkey_client.get(f"import_progress:{user_id}")
        if progress_json:
            progress = json.loads(progress_json)
            progress.update({
                "current": current,
                "current_podcast": current_podcast
            })
            valkey_client.set(f"import_progress:{user_id}", json.dumps(progress))

    def get_progress(self, user_id: int) -> Tuple[int, int, str]:
        progress_json = valkey_client.get(f"import_progress:{user_id}")
        if progress_json:
            progress = json.loads(progress_json)
            return (progress.get("current", 0), 
                    progress.get("total", 0), 
                    progress.get("current_podcast", ""))
        return (0, 0, "")

    def clear_progress(self, user_id: int):
        valkey_client.delete(f"import_progress:{user_id}")

import_progress_manager = ImportProgressManager()