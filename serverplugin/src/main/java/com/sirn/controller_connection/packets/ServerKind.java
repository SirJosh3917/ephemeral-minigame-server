package com.sirn.controller_connection.packets;

import com.fasterxml.jackson.annotation.JsonInclude;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class ServerKind {
    public String tag;
    public MinigamePayload payload;

    public static ServerKind PROXY;
    public static ServerKind LOBBY;

    static {
        PROXY = new ServerKind();
        PROXY.tag = "Proxy";

        LOBBY = new ServerKind();
        LOBBY.tag = "Lobby";
    }

    public static ServerKind minigame(String minigameKind) {
        MinigamePayload payload = new MinigamePayload();
        payload.kind = minigameKind;

        ServerKind serverKind = new ServerKind();
        serverKind.tag = "Minigame";
        serverKind.payload = payload;
        return serverKind;
    }
}
