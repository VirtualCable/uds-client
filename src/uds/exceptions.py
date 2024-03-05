class UDSException(Exception):
    pass


class MessageException(UDSException):
    pass


class ArgumentException(UDSException):
    pass


class RetryException(UDSException):
    pass


class InvalidVersionException(UDSException):
    link: str

    def __init__(self, client_link: str, required_version: str) -> None:
        super().__init__(client_link)
        self.link = client_link
        self.required_version = required_version
