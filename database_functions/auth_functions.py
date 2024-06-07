from passlib.context import CryptContext

# Create a Passlib context for Argon2
pwd_context = CryptContext(schemes=["argon2"], deprecated="auto")

def hash_password(password: str):
    # Use the Passlib context to hash the password
    hashed_password = pwd_context.hash(password)
    return hashed_password

def verify_password(cnx, database_type, username: str, password: str) -> bool:
    print("preparing pw check")
    if database_type == "postgresql":
        cursor = cnx.cursor()
        cursor.execute('SELECT Hashed_PW FROM "Users" WHERE Username = %s', (username,))
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(buffered=True)
        cursor.execute("SELECT Hashed_PW FROM Users WHERE Username = %s", (username,))

    result = cursor.fetchone()
    cursor.close()
    print("ran pw get")

    if not result:
        print("User not found")
        return False  # User not found

    stored_hashed_password = result[0] if isinstance(result, tuple) else result["hashed_pw"] if result and "hashed_pw" in result else 0
        # Check the type of the result and access the is_admin value accordingly
    # is_admin = is_admin_result[0] if isinstance(is_admin_result, tuple) else is_admin_result["IsAdmin"] if is_admin_result else 0

    print(f"Stored hashed password: {stored_hashed_password}")

    try:
        # Use the Passlib context to verify the password against the stored hash
        is_valid = pwd_context.verify(password, stored_hashed_password)
        print(f"Password verification result: {is_valid}")
        return is_valid
    except Exception as e:
        print(f"Error verifying password: {e}")
        return False