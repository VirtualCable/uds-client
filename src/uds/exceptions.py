class UDSException(Exception):
    pass


class MessageException(UDSException):
    pass


class ArgumentException(UDSException):
    pass


class RetryException(UDSException):
    pass


class InvalidVersion(UDSException):
    downloadUrl: str

    def __init__(self, downloadUrl: str) -> None:
        super().__init__(downloadUrl)
        self.downloadUrl = downloadUrl
