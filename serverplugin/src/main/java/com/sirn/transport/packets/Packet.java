package com.sirn.transport.packets;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Poor man's discriminated union. Only one of the public fields is guaranteed to be non-null.
 *
 * For documentation on packets, see `/controller/src/transport.rs`.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class Packet {
    @JsonProperty(value = "Authentication")
    public AuthenticationPacket authenticationPacket;

    @JsonProperty(value = "LinkServer")
    public LinkServerPacket linkServerPacket;

    @JsonProperty(value = "UnlinkServer")
    public UnlinkServerPacket unlinkServerPacket;

    @JsonProperty(value = "TransportPlayer")
    public TransportPlayerPacket transportPlayerPacket;

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

    public static RequestPacket makeRequestMinigame(String minigameKind, String playerUuid) {
        return new RequestPacket(AuthenticationKind.minigame(minigameKind), playerUuid);
    }

    @Override
    public String toString() {
        return "Packet{" +
                "authenticationPacket=" + authenticationPacket +
                ", linkServerPacket=" + linkServerPacket +
                ", unlinkServerPacket=" + unlinkServerPacket +
                ", transportPlayerPacket=" + transportPlayerPacket +
                ", requestPacket=" + requestPacket +
                ", pingPacket=" + pingPacket +
                ", pongPacket=" + pongPacket +
                ", updateActivePacket=" + updateActivePacket +
                '}';
    }
}
