-- Rooms table
CREATE TABLE rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT,
    is_private BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Room members table
CREATE TABLE room_members (
    room_id UUID REFERENCES rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL, -- This would refer to auth.users.id
    PRIMARY KEY (room_id, user_id)
);

-- Messages table
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID REFERENCES rooms(id) ON DELETE CASCADE,
    sender_id UUID NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Function to find or create a private room between two users
CREATE OR REPLACE FUNCTION get_private_room(user1 UUID, user2 UUID)
RETURNS UUID AS $$
DECLARE
    r_id UUID;
BEGIN
    SELECT rm1.room_id INTO r_id
    FROM room_members rm1
    JOIN room_members rm2 ON rm1.room_id = rm2.room_id
    JOIN rooms r ON r.id = rm1.room_id
    WHERE r.is_private = TRUE
      AND rm1.user_id = user1
      AND rm2.user_id = user2
    LIMIT 1;

    RETURN r_id;
END;
$$ LANGUAGE plpgsql;
