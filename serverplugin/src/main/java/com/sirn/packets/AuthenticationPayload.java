package com.sirn.packets;

public class AuthenticationPayload {
    public String kind;

    @Override
    public String toString() {
        return "AuthenticationPayload{" +
                "kind='" + kind + '\'' +
                '}';
    }
}
