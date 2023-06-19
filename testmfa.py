import pyotp

# Generate a random secret
totp = pyotp.TOTP(pyotp.random_base32())
print(totp)

# The OTP that the user would input to your application
print(totp.now())

# Verify an OTP against the current time
assert totp.verify(totp.now())
