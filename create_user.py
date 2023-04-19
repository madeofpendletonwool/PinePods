import sys
import mysql.connector
import database_functions.functions
import Auth.Passfunctions

if __name__ == "__main__":



	database_user = sys.argv[1] 
	database_pass = sys.argv[2]
	database_host = sys.argv[3]
	database_name = sys.argv[4]
	database_port = sys.argv[5]

	if len(sys.argv) > 6:
		fullname = sys.argv[6]
	else:
		fullname = "Pinepods Admin"

	if len(sys.argv) > 7:
		username = sys.argv[7]
	else:
		username = "admin"

	if len(sys.argv) > 8:
		email = sys.argv[8]
	else:
		email = "admin@pinepods.online"

	if len(sys.argv) > 9:
		password = sys.argv[9]
	else:
		alphabet = string.ascii_letters + string.digits + string.punctuation
		password = ''.join(secrets.choice(alphabet) for _ in range(15))


	salt, hash_pw = Auth.Passfunctions.hash_password(password)
	user_values = (fullname, username, email, hash_pw, salt)

	cnx = mysql.connector.connect(user=database_user, password=database_pass, host=database_host, port=database_port, database=database_name)

	if not database_functions.functions.user_exists(cnx, username):
		salt, hash_pw = Auth.Passfunctions.hash_password(password)
		user_values = (fullname, username, email, hash_pw, salt)
		print(f'Created Admin User = fullname={fullname}, username={username}, email={email}, password={password}')
		database_functions.functions.add_admin_user(cnx, user_values)
	else:
		print(f'Admin user "{username}" already exists.')

	cnx.close()