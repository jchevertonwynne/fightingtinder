table! {
    matches (username1, username2) {
        username1 -> Varchar,
        username2 -> Varchar,
    }
}

table! {
    swipes (swiper, swiped) {
        swiper -> Varchar,
        swiped -> Varchar,
        status -> Bool,
    }
}

table! {
    users (username) {
        username -> Varchar,
        password -> Varchar,
        lat -> Nullable<Float8>,
        long -> Nullable<Float8>,
        bio -> Nullable<Varchar>,
        profile_pic -> Nullable<Varchar>,
    }
}

allow_tables_to_appear_in_same_query!(matches, swipes, users,);
