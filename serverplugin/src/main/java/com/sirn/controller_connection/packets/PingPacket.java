package com.sirn.controller_connection.packets;

public class PingPacket {
    public int timer;

    @Override
    public String toString() {
        return "PingPacket{" +
                "timer=" + timer +
                '}';
    }
}
