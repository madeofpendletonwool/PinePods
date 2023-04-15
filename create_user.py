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
	fullname = sys.argv[6]
	username = sys.argv[7]
	email = sys.argv[8]
	password = sys.argv[9]

	salt, hash_pw = Auth.Passfunctions.hash_password(password)
	user_values = (fullname, username, email, hash_pw, salt)
	cnx = mysql.connector.connect(user=database_user, password=database_pass, host=database_host, port=database_port, database=database_name)
	database_functions.functions.add_admin_user(cnx, user_values)
	cnx.close()