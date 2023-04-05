package com.sirn.packets;

public class TransportPlayerPacket {
    public String player;
    public String to;

    @Override
    public String toString() {
        return "TransportPlayerPacket{" +
                "player='" + player + '\'' +
                ", to='" + to + '\'' +
                '}';
    }
}
