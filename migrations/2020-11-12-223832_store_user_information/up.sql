ALTER TABLE users
ADD COLUMN bio VARCHAR;
CREATE TABLE images (
    id SERIAL PRIMARY KEY,
    username VARCHAR NOT NULL,
    CONSTRAINT user_fk
        FOREIGN KEY(username)
            REFERENCES users(username)
);
CREATE INDEX ON images(username)