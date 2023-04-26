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
    print('checking pw')
    cursor = cnx.cursor()
    cursor.execute("SELECT Hashed_PW, Salt FROM Users WHERE Username = %s", (username,))
    result = cursor.fetchone()
    cursor.close()
    if not result:
        return False  # user not found

    hashed_password = result[0].encode('utf-8')
    salt = result[1].encode('utf-8')

    # Hash the password with the stored salt
    password_hash = bcrypt.hashpw(password.encode('utf-8'), salt)

    # Compare the hashed password with the stored hash
    return password_hash == hashed_password