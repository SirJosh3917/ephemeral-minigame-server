package com.sirn.head_controller.packets;

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
