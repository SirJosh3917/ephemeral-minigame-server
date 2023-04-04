package com.sirn.head_controller.packets;

import com.fasterxml.jackson.annotation.JsonInclude;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class AuthenticationKind {
    public String tag;
    public AuthenticationPayload payload;
}
