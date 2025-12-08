using WixToolset.Dtf.WindowsInstaller;

namespace MyCSharpCustomAction;

/// <summary>
/// Result monad for railway-oriented error handling.
/// Similar to Rust's Result&lt;T, E&gt; or F#'s Result type.
/// </summary>
public readonly record struct Result<T>
{
    private readonly T? _value;
    private readonly string? _error;
    private readonly bool _isSuccess;

    private Result(T value) => (_value, _error, _isSuccess) = (value, null, true);
    private Result(string error) => (_value, _error, _isSuccess) = (default, error, false);

    public static Result<T> Ok(T value) => new(value);
    public static Result<T> Err(string error) => new(error);

    public bool IsSuccess => _isSuccess;
    public bool IsError => !_isSuccess;
    public T Value => _isSuccess ? _value! : throw new InvalidOperationException($"Cannot access Value on error: {_error}");
    public string Error => !_isSuccess ? _error! : throw new InvalidOperationException("Cannot access Error on success");

    /// <summary>Map: Transform the success value</summary>
    public Result<U> Map<U>(Func<T, U> f) => _isSuccess ? Result<U>.Ok(f(_value!)) : Result<U>.Err(_error!);

    /// <summary>Bind/FlatMap: Chain operations that return Result</summary>
    public Result<U> Bind<U>(Func<T, Result<U>> f) => _isSuccess ? f(_value!) : Result<U>.Err(_error!);

    /// <summary>Match: Pattern match on success or error</summary>
    public U Match<U>(Func<T, U> onSuccess, Func<string, U> onError) => _isSuccess ? onSuccess(_value!) : onError(_error!);

    /// <summary>Tap: Execute side effect on success without changing the value</summary>
    public Result<T> Tap(Action<T> action) { if (_isSuccess) action(_value!); return this; }

    /// <summary>TapError: Execute side effect on error</summary>
    public Result<T> TapError(Action<string> action) { if (!_isSuccess) action(_error!); return this; }

    public static implicit operator Result<T>(T value) => Ok(value);
}

/// <summary>Unit type for void-returning operations</summary>
public readonly record struct Unit
{
    public static readonly Unit Value = new();
}

/// <summary>
/// Modern C# custom actions for WiX installers using monadic error handling.
/// Demonstrates railway-oriented programming pattern.
/// </summary>
public class CustomActions
{
    /// <summary>
    /// Immediate custom action - runs with full session access.
    /// Uses Result monad for clean error handling without exceptions.
    /// </summary>
    [CustomAction]
    public static ActionResult SampleImmediateAction(Session session)
    {
        session.Log("=== Begin SampleImmediateAction ===");

        var result = GetProperty(session, "ProductName")
            .Tap(name => session.Log($"ProductName: {name}"))
            .Bind(_ => GetProperty(session, "INSTALLFOLDER"))
            .Tap(dir => session.Log($"INSTALLFOLDER: {dir}"))
            .Bind(_ => GetProperty(session, "MY_PROPERTY"))
            .Tap(prop => session.Log($"MY_PROPERTY: {prop}"))
            .Tap(prop =>
            {
                if (string.IsNullOrEmpty(prop))
                    session.Log("Warning: MY_PROPERTY is not set");
            })
            .Bind(_ => SetProperty(session, "CA_IMMEDIATE_RESULT", $"Processed at {DateTime.Now:O}"))
            .TapError(err => session.Log($"Error: {err}"));

        session.Log($"=== End SampleImmediateAction ({(result.IsSuccess ? "Success" : "Failure")}) ===");
        return result.Match(_ => ActionResult.Success, _ => ActionResult.Failure);
    }

    /// <summary>
    /// Deferred custom action - runs with elevated privileges during install.
    /// Demonstrates monadic handling of optional CustomActionData values.
    /// </summary>
    [CustomAction]
    public static ActionResult SampleDeferredAction(Session session)
    {
        session.Log("=== Begin SampleDeferredAction ===");

        var result = GetCustomActionData(session)
            .Tap(data =>
            {
                if (data.TryGetValue("InstallDir", out var installDir))
                    session.Log($"InstallDir from CustomActionData: {installDir}");

                if (data.TryGetValue("MyProperty", out var myProperty))
                    session.Log($"MyProperty from CustomActionData: {myProperty}");
            })
            .Map(_ => Unit.Value)
            .TapError(err => session.Log($"Error: {err}"));

        session.Log($"=== End SampleDeferredAction ({(result.IsSuccess ? "Success" : "Failure")}) ===");
        return result.Match(_ => ActionResult.Success, _ => ActionResult.Failure);
    }

    /// <summary>
    /// Rollback custom action - runs if installation fails after deferred action.
    /// Always returns success - rollback should not fail the uninstall.
    /// </summary>
    [CustomAction]
    public static ActionResult SampleRollbackAction(Session session)
    {
        session.Log("=== Begin SampleRollbackAction ===");

        _ = GetCustomActionData(session)
            .Tap(data =>
            {
                if (data.TryGetValue("InstallDir", out var installDir))
                    session.Log($"Rolling back changes in: {installDir}");
            })
            .TapError(err => session.Log($"Rollback warning (ignored): {err}"));

        // Rollback actions should always succeed
        session.Log("=== End SampleRollbackAction (Success) ===");
        return ActionResult.Success;
    }

    #region Helper Functions (pure, composable)

    private static Result<string> GetProperty(Session session, string name)
    {
        try
        {
            return Result<string>.Ok(session[name] ?? string.Empty);
        }
        catch (Exception ex)
        {
            return Result<string>.Err($"Failed to get property '{name}': {ex.Message}");
        }
    }

    private static Result<Unit> SetProperty(Session session, string name, string value)
    {
        try
        {
            session[name] = value;
            return Result<Unit>.Ok(Unit.Value);
        }
        catch (Exception ex)
        {
            return Result<Unit>.Err($"Failed to set property '{name}': {ex.Message}");
        }
    }

    private static Result<CustomActionData> GetCustomActionData(Session session)
    {
        try
        {
            return Result<CustomActionData>.Ok(session.CustomActionData);
        }
        catch (Exception ex)
        {
            return Result<CustomActionData>.Err($"Failed to get CustomActionData: {ex.Message}");
        }
    }

    #endregion
}
