package com.sirn.packets;

public class LinkServerPacket {
    public String name;
    public String address;
    public int port;
    public int priority;

    @Override
    public String toString() {
        return "LinkServerPacket{" +
                "name='" + name + '\'' +
                ", address='" + address + '\'' +
                ", port=" + port +
                ", priority=" + priority +
                '}';
    }
}
