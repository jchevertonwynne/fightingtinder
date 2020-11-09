CREATE TABLE matches (
    username1 VARCHAR NOT NULL,
    username2 VARCHAR NOT NULL,
    CONSTRAINT match_pk
        PRIMARY KEY(username1, username2),
    CONSTRAINT user1_fk
        FOREIGN KEY(username1)
            REFERENCES users(username),
    CONSTRAINT user2_fk
        FOREIGN KEY(username2)
            REFERENCES users(username),
    CONSTRAINT usernames_in_order
        CHECK (username1 < username2)
)