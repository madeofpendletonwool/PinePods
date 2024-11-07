import os

def setup_test_environment():
    """Set up test environment variables for database configuration"""
    os.environ['DB_TYPE'] = os.getenv('TEST_DB_TYPE', 'postgresql')
    os.environ['DB_HOST'] = '127.0.0.1'
    os.environ['DB_PORT'] = '5432' if os.getenv('DB_TYPE') == 'postgresql' else '3306'
    os.environ['DB_USER'] = 'test_user'
    os.environ['DB_PASSWORD'] = 'test_password'
    os.environ['DB_NAME'] = 'test_db'
    os.environ['TEST_MODE'] = 'True'
    os.environ['SEARCH_API_URL'] = 'https://search.pinepods.online/api/search'
    os.environ['PEOPLE_API_URL'] = 'https://people.pinepods.online/api/hosts'
