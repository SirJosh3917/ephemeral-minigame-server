package com.sirn.transport.packets;

public class UpdateActivePacket {
    public boolean active;

    public UpdateActivePacket(boolean active) {
        this.active = active;
    }

    @Override
    public String toString() {
        return "UpdateActivePacket{" +
                "active=" + active +
                '}';
    }
}
