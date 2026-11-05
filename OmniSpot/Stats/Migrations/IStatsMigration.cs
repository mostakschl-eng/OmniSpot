using LiteDB;

namespace OmniSpot.Stats.Migrations;

internal interface IStatsMigration
{
    int Version { get; }
    void Up(ILiteDatabase db);
}