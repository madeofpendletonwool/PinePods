import sqlite3

conn = sqlite3.connect('pypodssaved.db')

c = conn.cursor()

# c.execute("""CREATE TABLE podcasts (
#             podcasts text
#             )""")

# c.execute("INSERT INTO podcasts VALUES ('My Brother My Brother and Me')")

def get_pods():
    c.execute("SELECT * FROM podcasts")
    return c.fetchall()

print(get_pods())

conn.commit()

conn.close()