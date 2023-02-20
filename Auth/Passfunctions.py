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
    with cnx.cursor() as cursor:
        # Query the database to get the user's hashed password and salt
        cursor.execute("SELECT Hashed_PW, Salt FROM Users WHERE Username = %s", (username,))
        result = cursor.fetchone()
        if not result:
            return False  # user not found

        hashed_password = result[0].encode('utf-8')
        salt = result[1].encode('utf-8')

        # Hash the password with the stored salt
        password_hash = bcrypt.hashpw(password.encode('utf-8'), salt)

        # Compare the hashed password with the stored hash
        return password_hash == hashed_password
    



# password = 'pass123'
# salt, hashed_password = hash_password(password)

# print(salt)
# print(hashed_password)

# stored_salt = b'$2b$12$JYmVRRycF5bx94MIr4tb8O'
# stored_hash = b'$2b$12$JYmVRRycF5bx94MIr4tb8OJ2l4MCOt8DkoLumJQ8OVRRR6fnqY2T.'

# check_password = verify_password(password, stored_hash, stored_salt)


# print(check_password)