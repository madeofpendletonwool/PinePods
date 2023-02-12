import bcrypt


def hash_password(password: str):
    # Generate a random salt
    salt = bcrypt.gensalt()

    # Hash the password with the salt
    hashed_password = bcrypt.hashpw(password.encode('utf-8'), salt)

    # Return the salt and the hashed password
    return salt, hashed_password

def verify_password(password: str, hashed_password: str, salt: str):
    # Hash the password with the stored salt
    password_hash = bcrypt.hashpw(password.encode('utf-8'), salt.encode('utf-8'))

    # Compare the hashed password with the stored hash
    return password_hash == hashed_password


password = 'orange11'
salt, hashed_password = hash_password(password)

print(salt)
print(hashed_password)

stored_salt = b'$2b$12$JM7fly9kMP.g.PQ8pK7CHO'
stored_hash = b'$2b$12$JM7fly9kMP.g.PQ8pK7CHOPDnRUIflUdBW3EN9.3dUPLzW6BqKP.2'

check_password = verify_password(password, stored_hash, stored_salt)

print(check_password)