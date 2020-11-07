CREATE TABLE swipes (
    swiper VARCHAR NOT NULL,
    swiped VARCHAR NOT NULL,
    status BOOLEAN NOT NULL,
    CONSTRAINT swipe_pk
        PRIMARY KEY(swiper, swiped),
    CONSTRAINT swiper_fk
        FOREIGN KEY(swiper)
            REFERENCES users(username),
    CONSTRAINT swiped_fk
        FOREIGN KEY(swiped)
            REFERENCES users(username),
    CONSTRAINT not_self 
        CHECK (swiped != swiper)
)