import sys
import mysql.connector
import database_functions.functions
import Auth.Passfunctions


if __name__ == "__main__":
	database_user = sys.argv[1] 
	database_pass = sys.argv[2]
	database_host = sys.argv[3]
	database_name = sys.argv[4]
	fullname = sys.argv[5]
	username = sys.argv[6]
	email = sys.argv[7]
	password = sys.argv[8]

	salt, hash_pw = Auth.Passfunctions.hash_password(password)
	user_values = (fullname, username, email, hash_pw, salt)
	cnx = mysql.connector.connect(user=database_user, password=database_pass, host=database_host, database=database_name)
	user_values = (fullname, username, email, hash_pw, salt)
	database_functions.functions.add_user(cnx, user_values)
	cnx.close()