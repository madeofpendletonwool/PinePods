import mysql.connector

# Connect to the database
cnx = mysql.connector.connect(
    host="127.0.0.1",
    port="3306",
    user="root",
    password="password",
    database="pypods_database"
)

# Create a cursor object
cursor = cnx.cursor()

# Read the SQL script file into a string
with open("tablecreate.sql", "r") as file:
    table_setup = file.read()

# Execute the SQL script
cursor.execute(table_setup)

# Close the cursor
cursor.close()

# Commit the changes
cnx.commit()

# Close the cursor and connection
cursor.close()
cnx.close()