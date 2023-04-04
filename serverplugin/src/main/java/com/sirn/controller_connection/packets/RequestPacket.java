package com.sirn.controller_connection.packets;

import com.fasterxml.jackson.annotation.JsonInclude;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class RequestPacket {
    public ServerKind kind;
    public String player;

    public RequestPacket(ServerKind kind, String player) {
        this.kind = kind;
        this.player = player;
    }

    @Override
    public String toString() {
        return "RequestPacket{" +
                "kind=" + kind +
                ", player='" + player + '\'' +
                '}';
    }
}
