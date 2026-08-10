
namespace AgainstRomeModifier
{
    public class EndlessAiModule
    {
        public string Id { get; }
        public string Name { get; }
        public List<IEndlessPatch> Patches { get; }

        public EndlessAiModule(string id, string name, List<IEndlessPatch> patches)
        {
            Id = id;
            Name = name;
            Patches = patches;
        }
    }
}
