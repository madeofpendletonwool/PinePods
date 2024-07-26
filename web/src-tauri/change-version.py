import json
import sys

def update_version(file_path, new_version):
    with open(file_path, 'r') as file:
        config = json.load(file)

    config['package']['version'] = new_version

    with open(file_path, 'w') as file:
        json.dump(config, file, indent=2)

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: update_version.py <file_path> <new_version>")
        sys.exit(1)

    file_path = sys.argv[1]
    new_version = sys.argv[2]

    update_version(file_path, new_version)
