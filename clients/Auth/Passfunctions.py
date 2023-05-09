import bcrypt
import mysql.connector


def hash_password(password: str):
    # Generate a random salt
    salt = bcrypt.gensalt()

    # Hash the password with the salt
    hashed_password = bcrypt.hashpw(password.encode('utf-8'), salt)

    # Return the salt and the hashed password
    return salt, hashed_password

# def verify_password(password: str, hashed_password: str, salt: bytes):
#     # Hash the password with the stored salt
#     password_hash = bcrypt.hashpw(password.encode('utf-8'), salt)

#     # Compare the hashed password with the stored hash
#     return password_hash == hashed_password

def verify_password(cnx, username: str, password: str) -> bool:
    cursor = cnx.cursor()
    print('checking pw')
    cursor.execute("SELECT Hashed_PW, Salt FROM Users WHERE Username = %s", (username,))
    result = cursor.fetchone()
    cursor.close()
    if not result:
        return False  # user not found

    hashed_password = result[0].encode('utf-8')
    salt = result[1].encode('utf-8')

    print(f"Stored hashed_password: {hashed_password}")
    print(f"Stored salt: {salt}")

    # Hash the password with the stored salt
    password_hash = bcrypt.hashpw(password.encode('utf-8'), salt)

    print(f"Generated password_hash: {password_hash}")

    # Compare the hashed password with the stored hash
    print(password_hash == hashed_password)
    return password_hash == hashed_password
