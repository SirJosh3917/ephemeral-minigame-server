package com.sirn.transport.packets;

public class AuthenticationPayload {
    public String kind;

    @Override
    public String toString() {
        return "AuthenticationPayload{" +
                "kind='" + kind + '\'' +
                '}';
    }
}
