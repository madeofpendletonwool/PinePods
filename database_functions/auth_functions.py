from passlib.context import CryptContext

# Create a Passlib context for Argon2
pwd_context = CryptContext(schemes=["argon2"], deprecated="auto")

def hash_password(password: str):
    # Use the Passlib context to hash the password
    hashed_password = pwd_context.hash(password)
    return hashed_password

def verify_password(cnx, username: str, password: str) -> bool:
    cursor = cnx.cursor(buffered=True)
    print('checking pw')
    cursor.execute("SELECT Hashed_PW FROM Users WHERE Username = %s", (username,))
    result = cursor.fetchone()
    cursor.close()

    if not result:
        return False  # User not found

    stored_hashed_password = result[0]

    # Use the Passlib context to verify the password against the stored hash
    return pwd_context.verify(password, stored_hashed_password)
