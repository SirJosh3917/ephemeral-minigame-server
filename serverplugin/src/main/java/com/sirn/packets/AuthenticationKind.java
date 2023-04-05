package com.sirn.packets;

import com.fasterxml.jackson.annotation.JsonInclude;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class AuthenticationKind {
    public String tag;
    public AuthenticationPayload payload;

    public static AuthenticationKind PROXY;
    public static AuthenticationKind LOBBY;

    static {
        PROXY = new AuthenticationKind();
        PROXY.tag = "Proxy";

        LOBBY = new AuthenticationKind();
        LOBBY.tag = "Lobby";
    }

    public static AuthenticationKind minigame(String minigameKind) {
        AuthenticationPayload payload = new AuthenticationPayload();
        payload.kind = minigameKind;

        AuthenticationKind serverKind = new AuthenticationKind();
        serverKind.tag = "Minigame";
        serverKind.payload = payload;
        return serverKind;
    }
}
