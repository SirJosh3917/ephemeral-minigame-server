package com.sirn.packets;

public class PingPacket {
    public int timer;

    @Override
    public String toString() {
        return "PingPacket{" +
                "timer=" + timer +
                '}';
    }
}
