package com.sirn.controller_connection.packets;

public class AuthenticationPacket {
    public String name;
    public ServerKind kind;
    public String ip;

    @Override
    public String toString() {
        return "AuthenticationPacket{" +
                "name='" + name + '\'' +
                ", kind=" + kind +
                ", ip='" + ip + '\'' +
                '}';
    }
}
