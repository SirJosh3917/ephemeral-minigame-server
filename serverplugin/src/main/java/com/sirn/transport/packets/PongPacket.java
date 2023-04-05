package com.sirn.transport.packets;

public class PongPacket {
    public int timer;

    public PongPacket(int timer) {
        this.timer = timer;
    }

    @Override
    public String toString() {
        return "PongPacket{" +
                "timer=" + timer +
                '}';
    }
}
