using System.Diagnostics.CodeAnalysis;

namespace OmniSpot.Configuration;

internal record ValidationResult
{
    private ValidationResult(ValidationResultStatus Status, string? Process, string? Message)
    {
        this.Status = Status;
        this.Process = Process;
        this.Message = Message;
    }

    [MemberNotNullWhen(true, nameof(Message))]
    public bool IsError => Status == ValidationResultStatus.Error;

    public ValidationResultStatus Status { get; }
    public string? Process { get; }
    public string? Message { get; }

    public static ValidationResult Success => new(ValidationResultStatus.Success, null, null);
    public static ValidationResult Error(string message) => new(ValidationResultStatus.Error, null, message);
    public static ValidationResult Error(string process, string message) => new(ValidationResultStatus.Error, process, message);
}

internal enum ValidationResultStatus
{
    Success,
    Error
}
