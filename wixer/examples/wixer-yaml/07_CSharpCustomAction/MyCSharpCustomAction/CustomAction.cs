using WixToolset.Dtf.WindowsInstaller;

namespace MyCSharpCustomAction;

/// <summary>
/// Modern C# custom actions for WiX installers.
/// Targets .NET 6 (WiX v6) and .NET 8 (WiX mainline/v7).
/// </summary>
public class CustomActions
{
    /// <summary>
    /// Immediate custom action - runs with full session access.
    /// Can read/write properties, access the database, etc.
    /// </summary>
    [CustomAction]
    public static ActionResult SampleImmediateAction(Session session)
    {
        session.Log("=== Begin SampleImmediateAction ===");
        
        try
        {
            // Read properties
            var productName = session["ProductName"];
            var installDir = session["INSTALLFOLDER"];
            var myProperty = session["MY_PROPERTY"];
            
            session.Log($"ProductName: {productName}");
            session.Log($"INSTALLFOLDER: {installDir}");
            session.Log($"MY_PROPERTY: {myProperty}");
            
            // Set a property for later use
            session["CA_IMMEDIATE_RESULT"] = $"Processed at {DateTime.Now:O}";
            
            // Example: Validate something
            if (string.IsNullOrEmpty(myProperty))
            {
                session.Log("Warning: MY_PROPERTY is not set");
            }
            
            session.Log("=== End SampleImmediateAction (Success) ===");
            return ActionResult.Success;
        }
        catch (Exception ex)
        {
            session.Log($"Error in SampleImmediateAction: {ex}");
            return ActionResult.Failure;
        }
    }

    /// <summary>
    /// Deferred custom action - runs with elevated privileges during install.
    /// Cannot access session properties directly; must use CustomActionData.
    /// </summary>
    [CustomAction]
    public static ActionResult SampleDeferredAction(Session session)
    {
        session.Log("=== Begin SampleDeferredAction ===");
        
        try
        {
            // Deferred actions use CustomActionData (set by a preceding immediate action)
            var data = session.CustomActionData;
            
            if (data.TryGetValue("InstallDir", out var installDir))
            {
                session.Log($"InstallDir from CustomActionData: {installDir}");
                
                // Example: Create a file in the install directory
                // In a real CA, you might configure services, set permissions, etc.
            }
            
            if (data.TryGetValue("MyProperty", out var myProperty))
            {
                session.Log($"MyProperty from CustomActionData: {myProperty}");
            }
            
            session.Log("=== End SampleDeferredAction (Success) ===");
            return ActionResult.Success;
        }
        catch (Exception ex)
        {
            session.Log($"Error in SampleDeferredAction: {ex}");
            return ActionResult.Failure;
        }
    }

    /// <summary>
    /// Rollback custom action - runs if installation fails after deferred action.
    /// Used to undo changes made by the deferred action.
    /// </summary>
    [CustomAction]
    public static ActionResult SampleRollbackAction(Session session)
    {
        session.Log("=== Begin SampleRollbackAction ===");
        
        try
        {
            var data = session.CustomActionData;
            
            if (data.TryGetValue("InstallDir", out var installDir))
            {
                session.Log($"Rolling back changes in: {installDir}");
                // Undo whatever the deferred action did
            }
            
            session.Log("=== End SampleRollbackAction (Success) ===");
            return ActionResult.Success;
        }
        catch (Exception ex)
        {
            session.Log($"Error in SampleRollbackAction: {ex}");
            // Rollback actions should generally succeed even on error
            return ActionResult.Success;
        }
    }
}
