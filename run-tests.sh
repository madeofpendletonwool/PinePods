#!/bin/bash

# Get the directory where the script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"


check_postgres() {
    if docker logs pinepods-test-db 2>&1 | grep -q "database system is ready to accept connections"; then
        echo "PostgreSQL is ready!"
        return 0
    else
        echo "PostgreSQL is not ready"
        return 1
    fi
}

# Function to start MySQL container
start_mysql() {
    if ! docker ps | grep -q pinepods-mysql-test; then
        if docker ps -a | grep -q pinepods-mysql-test; then
            docker start pinepods-mysql-test
        else
            docker run --name pinepods-mysql-test \
                -e MYSQL_USER=test_user \
                -e MYSQL_PASSWORD=test_password \
                -e MYSQL_DATABASE=test_db \
                -e MYSQL_RANDOM_ROOT_PASSWORD=yes \
                -p 3306:3306 \
                -d mariadb:latest

            # Wait for MySQL to be ready
            echo "Waiting for MySQL to be ready..."
            sleep 10
        fi
    fi
}

# Function to start PostgreSQL container
start_postgres() {
    if ! docker ps | grep -q pinepods-test-db; then
        if docker ps -a | grep -q pinepods-test-db; then
            docker start pinepods-test-db
        else
            docker run --name pinepods-test-db \
                -e POSTGRES_USER=test_user \
                -e POSTGRES_PASSWORD=test_password \
                -e POSTGRES_DB=test_db \
                -p 5432:5432 \
                -d postgres:latest

            # Wait for PostgreSQL to be ready
            echo "Waiting for PostgreSQL to be ready..."
            sleep 10
            check_postgres
        fi
    fi
}

# Function to setup database schema
setup_database() {
    local db_type=$1

    # Export necessary environment variables for the setup script
    export DB_HOST=localhost
    export DB_PORT=5432
    export DB_USER=test_user
    export DB_PASSWORD=test_password
    export DB_NAME=test_db
    export DB_TYPE=$db_type

    echo "Setting up $db_type schema using new migration system..."
    python3 "${SCRIPT_DIR}/startup/setup_database_new.py"
}



# Activate virtual environment
source "${SCRIPT_DIR}/venv/bin/activate"

# Set PYTHONPATH to include the project root
export PYTHONPATH="${SCRIPT_DIR}:${PYTHONPATH}"

# Parse command line arguments
DB_TYPE=${1:-"postgresql"}  # Default to PostgreSQL if no argument provided

case $DB_TYPE in
    "postgresql")
        echo "Running tests with PostgreSQL..."
        start_postgres
        export TEST_DB_TYPE=postgresql
        setup_database postgresql
        ;;
    "mariadb")
        echo "Running tests with MariaDB..."
        start_mysql
        export TEST_DB_TYPE=mariadb
        setup_database mariadb
        ;;
    *)
        echo "Invalid database type. Use 'postgresql' or 'mariadb'"
        exit 1
        ;;
esac

# Function to cleanup containers
cleanup_containers() {
    local db_type=$1
    if [ "$db_type" == "postgresql" ]; then
        echo "Cleaning up PostgreSQL container..."
        docker stop pinepods-test-db >/dev/null 2>&1
        docker rm pinepods-test-db >/dev/null 2>&1
    else
        echo "Cleaning up MariaDB container..."
        docker stop pinepods-mysql-test >/dev/null 2>&1
        docker rm pinepods-mysql-test >/dev/null 2>&1
    fi
}

# Add trap to ensure cleanup happens even if script fails
trap 'cleanup_containers "$DB_TYPE"' EXIT

# Rest of your script remains the same...

# Run tests with verbose output
pytest -v tests/

# Cleanup will happen automatically due to trap, but you can also call it explicitly
cleanup_containers "$DB_TYPE"
