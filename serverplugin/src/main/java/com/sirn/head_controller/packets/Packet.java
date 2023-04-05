package com.sirn.head_controller.packets;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;

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

    @Override
    public String toString() {
        return "Packet{" +
                "authenticationPacket=" + authenticationPacket +
                ", linkServerPacket=" + linkServerPacket +
                ", unlinkServerPacket=" + unlinkServerPacket +
                ", transportPlayerPacket=" + transportPlayerPacket +
                '}';
    }
}
