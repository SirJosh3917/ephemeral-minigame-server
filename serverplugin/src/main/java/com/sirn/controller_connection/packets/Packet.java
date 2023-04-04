package com.sirn.controller_connection.packets;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class Packet {
    @JsonProperty(value = "Authentication")
    public AuthenticationPacket authenticationPacket;

    @JsonProperty(value = "Request")
    public RequestPacket requestPacket;

    @JsonProperty(value = "Ping")
    public PingPacket pingPacket;

    @JsonProperty(value = "Pong")
    public PongPacket pongPacket;

    @JsonProperty(value = "UpdateActive")
    public UpdateActivePacket updateActivePacket;

    public Packet() {}

    public Packet(PongPacket pongPacket) {
        this.pongPacket = pongPacket;
    }

    public Packet(UpdateActivePacket updateActivePacket) {
        this.updateActivePacket = updateActivePacket;
    }

    public static Packet makeRequestMinigame(String minigameKind, String playerUuid) {
        Packet packet = new Packet();
        packet.requestPacket = new RequestPacket(ServerKind.minigame(minigameKind), playerUuid);
        return packet;
    }

    @Override
    public String toString() {
        return "Packet{" +
                "authenticationPacket=" + authenticationPacket +
                ", requestPacket=" + requestPacket +
                ", pingPacket=" + pingPacket +
                ", pongPacket=" + pongPacket +
                ", updateActivePacket=" + updateActivePacket +
                '}';
    }
}
